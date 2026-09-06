/*
Optimized on 6 Jan 2010 by pjn123
*/

#include <lib/std.mi>

Global Group g, g_volume, g_vis, g_buttons, g_ejectVol, g_volbig;
Global Layer l_left, l_right, l_volbig1, l_volbig2, l_leftOverlay, l_buttonbg_full, l_buttonbg_vol;
Global Button b_play, b_pause, b_eject, b_mute;
Global Guiobject gui_vis;
Global Slider s_volbig;
Global int _powerSave1;

System.onScriptLoaded() {
	g = getScriptGroup();
	g_vis = g.getObject("two.playback.vis");
	g_buttons = g.getObject("two.playback.buttons");
	g_volume = g.getObject("two.playback.volume");

	l_left = g_vis.getObject("two.playback.left");
	l_leftOverlay = g_vis.getObject("vis.overlay");
	gui_vis = g_vis.getObject("two.playback.visobject");
	
	b_play = g_buttons.getObject("two.playback.play");
	b_pause = g_buttons.getObject("two.playback.paus");
	b_eject = g_buttons.getObject("two.playback.eject");
	g_ejectVol = g_buttons.getObject("two.playback.ejectvol");

	l_right = g_volume.getObject("two.playback.right");
	g_volbig = g_volume.getObject("two.playback.volume.slider");
	b_mute = g_volume.getObject("two.player.mute");
	
	l_buttonbg_full = g_buttons.getObject("two.playback.button.full.bg");
	l_buttonbg_vol = g_buttons.getObject("two.playback.button.vol.bg");

	s_volbig = g_volbig.getObject("two.playback.volslider");
	
	
	// Play-Pause button
	if(System.getStatus() == STATUS_PLAYING){
		b_play.hide();
		b_pause.show();
	}
	else{
		b_play.show();
		b_pause.hide();
	}
	
	// Reader for classic vis colors from bitmap (wa5.51)
	Map myMap = new Map;
	myMap.loadMap("cpro2.color.read");
	
	gui_vis.setXmlParam("colorbandpeak",integerToString(myMap.getARGBValue(0,0,2))+","+integerToString(myMap.getARGBValue(0,0,1))+","+integerToString(myMap.getARGBValue(0,0,0)));
	
	for(int i=2;i<18;i++){
		gui_vis.setXmlParam("colorband"+integerToString(18-i),integerToString(myMap.getARGBValue(0,i,2))+","+integerToString(myMap.getARGBValue(0,i,1))+","+integerToString(myMap.getARGBValue(0,i,0)));
	}

	for(int i=0;i<5;i++){
		gui_vis.setXmlParam("colorosc"+integerToString(5-i),integerToString(myMap.getARGBValue(2,i,2))+","+integerToString(myMap.getARGBValue(2,i,1))+","+integerToString(myMap.getARGBValue(2,i,0)));
	}
	delete myMap;
}

g.onResize(int x, int y, int w, int h){
	if(w<334){
		if(_powerSave1!=1){
			g_vis.hide();
			g_volume.hide();
			
			b_eject.hide();
			g_ejectVol.show();
			b_mute.hide();
			
			l_buttonbg_full.setXmlParam("w", "191");
			l_buttonbg_vol.show();
			
			_powerSave1=1;
		}
	}
	else if(w<406){
		if(_powerSave1!=2){
			g_vis.show();
			g_volume.show();

			b_eject.show();
			g_ejectVol.hide();
			b_mute.hide();

			l_left.setXmlParam("image","playback.bg.left.3");
			l_leftOverlay.setXmlParam("image","vis.overlay3");
			l_leftOverlay.setXmlParam("w","35");
			g_vis.setXmlParam("w","47");
			gui_vis.setXmlParam("w","35");
			
			l_right.setXmlParam("image","playback.bg.right.3");
			g_volume.setXmlParam("w","47");
			g_volume.setXmlParam("x","-55");
			g_volbig.setXmlParam("x","3");
			g_volbig.setXmlParam("w","41");

			l_buttonbg_full.setXmlParam("w", "224");
			l_buttonbg_vol.hide();

			_powerSave1=2;
		}
	}
	else if(w<466){
		if(_powerSave1!=3){
			g_vis.show();
			g_volume.show();

			b_eject.show();
			g_ejectVol.hide();
			b_mute.hide();

			l_left.show();
			l_left.setXmlParam("image","playback.bg.left.2");
			l_leftOverlay.setXmlParam("image","vis.overlay2");
			l_leftOverlay.setXmlParam("w","71");
			g_vis.setXmlParam("w","83");
			gui_vis.setXmlParam("w","71");
			
			l_right.show();
			l_right.setXmlParam("image","playback.bg.right.2");
			g_volume.setXmlParam("w","83");
			g_volume.setXmlParam("x","-91");
			g_volbig.setXmlParam("x","3");
			g_volbig.setXmlParam("w","77");

			l_buttonbg_full.setXmlParam("w", "224");
			l_buttonbg_vol.hide();

			_powerSave1=3;
		}
	}
	else{
		if(_powerSave1!=4){
			g_vis.show();
			g_volume.show();

			b_eject.show();
			g_ejectVol.hide();
			b_mute.show();

			l_left.show();
			l_left.setXmlParam("image","playback.bg.left.1");
			l_leftOverlay.setXmlParam("image","vis.overlay1");
			l_leftOverlay.setXmlParam("w","71");
			g_vis.setXmlParam("w","113");
			gui_vis.setXmlParam("w","71");

			l_right.show();
			l_right.setXmlParam("image","playback.bg.right.1");
			g_volume.setXmlParam("w","113");
			g_volume.setXmlParam("x","-121");
			g_volbig.setXmlParam("x","33");
			g_volbig.setXmlParam("w","77");

			l_buttonbg_full.setXmlParam("w", "224");
			l_buttonbg_vol.hide();

			_powerSave1=4;
		}

	}
	g_buttons.setXmlParam("x", integerToString(w/2-112));
}

// Play-Pause button
System.onPlay(){
	b_play.hide();
	b_pause.show();
}
System.onStop(){
	b_play.show();
	b_pause.hide();
}
System.onPause(){
	b_play.show();
	b_pause.hide();
}
System.onResume(){
	b_play.hide();
	b_pause.show();
}

//Restart current track ;)
b_pause.onRightButtonDown(int x, int y){ //Winamp bug hack
	b_pause.setXmlParam("action", "");
}
b_pause.onRightButtonUp(int x, int y){ 
	System.play();
	complete;
	b_pause.setXmlParam("action", "PAUSE");
}