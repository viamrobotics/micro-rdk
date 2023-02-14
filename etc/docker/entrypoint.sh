#!/bin/bash

if [[ -e /var/run/docker.sock ]]; then
    groupmod --gid $(ls -n /var/run/docker.sock | cut -d" " -f4) docker 
fi


REALARGS=()
while [[ $# -gt 0 ]]; do
    key="$1"
    case $key in
	--testbot-uid)
	    uid="$2"
	    shift # past argument
	    shift # past value
	    ;;
	--testbot-gid)
	    gid="$2"
	    shift # past argument
	    shift # past value
	    ;;
	*)    # unknown option
	    REALARGS+=(${1@Q})
	    shift # past argument
	    ;;
    esac
done

if [[ -n $uid ]]; then
    usermod --non-unique --uid $uid testbot >/dev/null
fi

if [[ -n $gid ]]; then
    groupmod --non-unique --gid $gid testbot >/dev/null
fi

if [[ -n $uid ]] || [[ -n $gid ]]; then
    if [[ -e /run/host-services/ssh-auth.sock ]]; then
	chown testbot:testbot /run/host-services/ssh-auth.sock
    fi

    cat >> /home/testbot/.bashrc <<-EOS

	ssh-add -l
	ret=\$?
	if [[ \$ret -ge 2 ]]; then
	  eval \$(ssh-agent)
	  ssh-add
	elif [[ \$ret -eq 1 ]]; then
	  ssh-add
	fi

	if ! grep -qs github.com ~/.netrc; then
	  ssh git@github.com
	  if [ \$? -eq 1 ]; then
	    git config --global url.ssh://git@github.com/.insteadOf https://github.com/
	  fi
	fi
	EOS
    sudo --preserve-env=SSH_AUTH_SOCK -u testbot bash -lc "${REALARGS[*]}"
else
    sudo -u testbot bash -lc "${REALARGS[*]}"
fi

