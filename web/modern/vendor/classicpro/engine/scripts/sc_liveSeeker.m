/*---------------------------------------------------
-----------------------------------------------------
Filename:	sc_liveSeeker.m

Type:		maki/source
Version:	skin version 1.2
Date:		15:18 20.08.2007
Author:		Pieter Nieuwoudt aka pjn123
E-Mail:		sylvester@skinconsortium.com
Internet:	-
-----------------------------------------------------
-----------------------------------------------------

How to use?

	<slider id="slider.seeker" x="23" y="-64" w="-47" h="16" relaty="1" relatw="1" alpha="150" action="SEEK" thumb="seeker.thumb.1" hoverThumb="seeker.thumb.2" downThumb="seeker.thumb.3" tooltip="Seeker"/>
	<script file="scripts/sc_liveSeeker.maki" param="slider.seeker"/>

Just like that. Very easy.

You can now go to the position in the track while you drag the seek slider if you hold down the alt button.
*/


#include <lib/std.mi>

Global Group frameGroup;
Global Slider seeker;
Global Boolean enableLiveSearch1, enableLiveSearch2, enableLiveSearch3;

System.onScriptLoaded() {
	frameGroup = getScriptGroup();
	seeker = frameGroup.findObject(System.getToken(System.getParam(), ";", 0));
}

seeker.onSetPosition(int newpos){
	if(enableLiveSearch1 && newpos%2==0){
		System.seekTo(getPlayItemLength()/255*newpos);
	}
	else if(enableLiveSearch2 && newpos%5==0){
			System.seekTo(getPlayItemLength()/255*newpos);
	}
	else if(enableLiveSearch3 && newpos%10==0){
			System.seekTo(getPlayItemLength()/255*newpos);
	}
}

System.onKeyDown(String key) {
	if (key == "ctrl"){
		enableLiveSearch1 = true;
	}
	else if (key == "shift"){
		enableLiveSearch2 = true;
	}
	else if (key == "alt"){
		enableLiveSearch3 = true;
	}
	else{
		enableLiveSearch1 = false;
		enableLiveSearch2 = false;
		enableLiveSearch3 = false;
	}
}