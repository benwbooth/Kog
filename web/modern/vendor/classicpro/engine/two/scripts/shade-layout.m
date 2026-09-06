/*
Optimized on 6 Jan 2010 by pjn123
*/

#include <lib/std.mi>
#include attribs/init_Autoresize.m

Function gotoGlobal();
Function saveGlobal();

Global Group g;
Global Button b_play, b_pause;

Global int lastKnownW;
Global Timer reCheck;
Global Container player;
Global Layout shade, normal;
Global Boolean docked;

System.onScriptLoaded() {
	initAttribs_Autoresize();

	player = System.getContainer("main");
	normal = player.getLayout("normal");
	shade = player.getLayout("shade");

	g = getScriptGroup();
	b_play = g.getObject("shade.play");
	b_pause = g.getObject("shade.pause");
	
	reCheck = new Timer;
	reCheck.setDelay(10);

	
	// Play-Pause button
	if(System.getStatus() == STATUS_PLAYING){
		b_play.hide();
		b_pause.show();
	}
	else{
		b_play.show();
		b_pause.hide();
	}
}
System.onScriptUnloading() {
	if(shade.isVisible()) saveGlobal();
	delete reCheck;
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


/*
Events order:
	::Going to NORMAL mode
	shade is open
	goto normal
	show normal [save position for SHADE here]
	hide shade [goto saved position for NORMAL here]

	::Going to SHADE mode
	normal is open
	goto shade
	show shade [save position for NORMAL here]
	hide normal [goto saved position for SHADE here]
*/

//Normal shows > Shade still open (so busy with switch) > Save pos for  normal mode
System.onShowLayout(Layout _layout){
	if(_layout==normal && shade.isVisible()){ 
		saveGlobal();
	}
	else if(_layout==shade && !normal.isVisible()){
		gotoGlobal(); //On cold start
	}
}

//Normal hides > Shade is open (so
System.onHideLayout(Layout _layout){
	if(_layout==normal && shade.isVisible()){
		gotoGlobal();
	}
}



gotoGlobal(){
	String piPrefix;
	if(linkPosWidth.getData() == "1") piPrefix = "cPro2.";
	else piPrefix = "cPro2.2";

	if(docked) return;
	int x = getPublicInt(piPrefix+"x", getCurAppLeft());
	int y = getPublicInt(piPrefix+"y", getCurAppTop());
	int w = getPublicInt(piPrefix+"w", 800);
	int h = 22;

	if(w<240) w= 240;
	
	//Winshade -> Normal : Bottom of screen
	if(getPublicInt("cPro2.saveby", 0)==0 && collapse_bottom_attrib.getData() == "1" && linkPosWidth.getData() != "0"){
		y=y+getPublicInt("cPro2.h", getCurAppHeight())-22;
	}

	shade.resize(x,y,w,h);
}
saveGlobal(){
	//if(player.getCurLayout() == shade){
		String piPrefix;
		if(linkPosWidth.getData() == "1") piPrefix = "cPro2.";
		else piPrefix = "cPro2.2";

		lastKnownW = shade.getWidth();
		
		setPublicInt(piPrefix+"x", shade.getLeft());
		setPublicInt(piPrefix+"y", shade.getTop());
		setPublicInt(piPrefix+"w", lastKnownW);
		//debug(piPrefix+" - "+integerToString(getPublicInt(piPrefix+"x", getCurAppLeft()))+","+integerToString(shade.getTop())+","+integerToString(lastKnownW));

		if(linkPosWidth.getData() == "1") setPublicInt("cPro2.saveby", 1); //0=normal ; 1=shade
	//}
}

reCheck.onTimer(){
		reCheck.stop();
		gotoGlobal();
}
shade.onDock(int side){
	docked=true;
	//aotBut.hide();
	//aotDoc.show();
}

shade.onUndock(){
	docked=false;
	//aotDoc.hide();
	//aotBut.show();
}

